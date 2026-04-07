use std::collections::HashMap;
use std::io::{Read, Write};
use std::time::{Duration, Instant};
use serialport::SerialPort;
use crate::constants::*;
use crate::protocol::*;

#[derive(Debug, Clone)]
pub struct TouchSensorPayload {
    pub input_port: usize,
    pub raw_value: u16,
    pub event: TouchEvent,
    pub pressed: bool,
    pub force: u8,
}

#[derive(Debug, Clone)]
pub struct TemperatureSensorPayload {
    pub input_port: usize,
    pub raw_value: u16,
    pub fahrenheit: f64,
    pub celsius: f64,
}

#[derive(Debug, Clone)]
pub struct LightSensorPayload {
    pub input_port: usize,
    pub raw_value: u16,
    pub intensity: u8,
}

#[derive(Debug, Clone)]
pub struct RotationSensorPayload {
    pub input_port: usize,
    pub raw_value: u16,
    pub rotations: i32,
    pub delta: i8,
}

#[derive(Debug, Clone)]
pub enum ControlLabSensorPayload {
    Touch(TouchSensorPayload),
    Temperature(TemperatureSensorPayload),
    Light(LightSensorPayload),
    Rotation(RotationSensorPayload),
}

pub struct ControlLab {
    path: String,
    baud_rate: u32,
    state: ControlLabState,
    port: Option<Box<dyn SerialPort>>,
    buffer: Vec<u8>,
    sensor_types: [SensorType; INPUT_PORT_COUNT],
    sensor_values: [i32; INPUT_PORT_COUNT],
    rotation_values: [i32; INPUT_PORT_COUNT],
    last_sensor_payloads: HashMap<String, ControlLabSensorPayload>,
    last_keep_alive: Instant,
}

impl ControlLab {
    pub fn new(path: &str) -> Self {
        ControlLab {
            path: path.to_string(),
            baud_rate: DEFAULT_BAUD_RATE,
            state: ControlLabState::NotReady,
            port: None,
            buffer: Vec::new(),
            sensor_types: [SensorType::Unknown; INPUT_PORT_COUNT],
            sensor_values: [0; INPUT_PORT_COUNT],
            rotation_values: [0; INPUT_PORT_COUNT],
            last_sensor_payloads: HashMap::new(),
            last_keep_alive: Instant::now(),
        }
    }

    pub fn state(&self) -> ControlLabState {
        self.state
    }

    pub fn is_connected(&self) -> bool {
        self.state == ControlLabState::Ready && self.port.is_some()
    }

    pub fn connect(&mut self) -> Result<(), String> {
        let port = serialport::new(&self.path, self.baud_rate)
            .timeout(Duration::from_secs(5))
            .open()
            .map_err(|e| format!("Failed to open serial port {}: {}", self.path, e))?;

        self.port = Some(port);
        self.buffer.clear();

        // Send handshake
        self.write_bytes(HANDSHAKE_OUTBOUND)?;

        // Wait for handshake response
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            self.read_available()?;
            if let Some(idx) = self.find_in_buffer(HANDSHAKE_INBOUND) {
                self.buffer = self.buffer[idx + HANDSHAKE_INBOUND.len()..].to_vec();
                self.state = ControlLabState::Ready;
                self.last_keep_alive = Instant::now();
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        Err("Control Lab handshake timed out".to_string())
    }

    pub fn disconnect(&mut self) {
        self.port = None;
        self.state = ControlLabState::NotReady;
        self.buffer.clear();
    }

    pub fn set_power(&mut self, output_port: &str, power: i8) -> Result<(), String> {
        if !self.is_connected() {
            return Err("Control Lab not connected".to_string());
        }
        if power < -8 || power > 8 {
            return Err("Power must be between -8 and 8".to_string());
        }
        let mask = get_output_port_mask(output_port)
            .ok_or_else(|| format!("Unknown output port '{}'", output_port))?;
        let cmd = encode_output_power(mask, power);
        self.write_bytes(&cmd)
    }

    /// Set power for multiple ports in a single serial write using combined bitmask.
    /// All ports must share the same power value.
    pub fn set_power_masked(&mut self, mask: u8, power: i8) -> Result<(), String> {
        if !self.is_connected() {
            return Err("Control Lab not connected".to_string());
        }
        if power < -8 || power > 8 {
            return Err("Power must be between -8 and 8".to_string());
        }
        let cmd = encode_output_power(mask, power);
        self.write_bytes(&cmd)
    }

    pub fn set_sensor_type(&mut self, input_port: usize, sensor_type: SensorType) {
        if input_port >= 1 && input_port <= INPUT_PORT_COUNT {
            let idx = input_port - 1;
            self.sensor_types[idx] = sensor_type;
            self.sensor_values[idx] = 0;
            self.rotation_values[idx] = 0;
        }
    }

    pub fn reset_rotation(&mut self, input_port: usize) {
        if input_port >= 1 && input_port <= INPUT_PORT_COUNT {
            self.rotation_values[input_port - 1] = 0;
        }
    }

    pub fn read(&self, input_port: usize) -> Option<ControlLabSensorPayload> {
        if input_port < 1 || input_port > INPUT_PORT_COUNT { return None; }
        let idx = input_port - 1;
        let sensor_type = self.sensor_types[idx];
        let kind = match sensor_type {
            SensorType::Touch => "touch",
            SensorType::Temperature => "temperature",
            SensorType::Light => "light",
            SensorType::Rotation => "rotation",
            SensorType::Unknown => return None,
        };
        let key = format!("{}:{}", kind, input_port);
        self.last_sensor_payloads.get(&key).cloned()
    }

    /// Poll for incoming sensor data. Call this regularly.
    pub fn poll(&mut self) -> Result<(), String> {
        if !self.is_connected() { return Ok(()); }

        self.read_available()?;
        self.process_sensor_messages();

        // Send keep-alive if needed
        if self.last_keep_alive.elapsed() >= Duration::from_millis(KEEP_ALIVE_INTERVAL_MS) {
            let ka = encode_keep_alive();
            self.write_bytes(&ka)?;
            self.last_keep_alive = Instant::now();
        }

        Ok(())
    }

    fn write_bytes(&mut self, data: &[u8]) -> Result<(), String> {
        if let Some(ref mut port) = self.port {
            port.write_all(data).map_err(|e| format!("Write failed: {}", e))?;
        }
        Ok(())
    }

    fn read_available(&mut self) -> Result<(), String> {
        if let Some(ref mut port) = self.port {
            let mut buf = [0u8; 256];
            match port.read(&mut buf) {
                Ok(n) if n > 0 => self.buffer.extend_from_slice(&buf[..n]),
                Ok(_) => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(format!("Read failed: {}", e)),
            }
        }
        Ok(())
    }

    fn find_in_buffer(&self, needle: &[u8]) -> Option<usize> {
        self.buffer.windows(needle.len()).position(|w| w == needle)
    }

    fn process_sensor_messages(&mut self) {
        while self.buffer.len() >= SENSOR_MESSAGE_LENGTH {
            if self.buffer[0] != 0x00 {
                self.buffer.remove(0);
                continue;
            }

            let message: Vec<u8> = self.buffer[..SENSOR_MESSAGE_LENGTH].to_vec();
            if let Some(notification) = decode_sensor_message(&message) {
                self.buffer = self.buffer[SENSOR_MESSAGE_LENGTH..].to_vec();
                self.handle_sensor_notification(&notification);
            } else {
                self.buffer.clear();
                return;
            }
        }
    }

    fn handle_sensor_notification(&mut self, notification: &SensorNotification) {
        for sample in &notification.samples {
            let port = sample.input_port;
            if port < 1 || port > INPUT_PORT_COUNT { continue; }
            let idx = port - 1;

            self.rotation_values[idx] += sample.rotation_delta as i32;
            let sensor_type = self.sensor_types[idx];

            match sensor_type {
                SensorType::Touch => {
                    let pressed_value = if sample.raw_value < 1000 {
                        TouchEvent::Pressed
                    } else {
                        TouchEvent::Released
                    };
                    let force = (100.0 - (sample.raw_value as f64 / 1024.0) * 100.0)
                        .max(0.0).min(100.0) as u8;
                    let payload = ControlLabSensorPayload::Touch(TouchSensorPayload {
                        input_port: port,
                        raw_value: sample.raw_value,
                        event: pressed_value,
                        pressed: pressed_value == TouchEvent::Pressed,
                        force,
                    });
                    self.last_sensor_payloads.insert(format!("touch:{}", port), payload);
                }
                SensorType::Temperature => {
                    let fahrenheit = ((760.0 - sample.raw_value as f64) / 4.4 + 32.0 * 100.0).round() / 100.0;
                    let celsius = (((760.0 - sample.raw_value as f64) / 4.4) * (5.0 / 9.0) * 100.0).round() / 100.0;
                    let payload = ControlLabSensorPayload::Temperature(TemperatureSensorPayload {
                        input_port: port,
                        raw_value: sample.raw_value,
                        fahrenheit,
                        celsius,
                    });
                    self.last_sensor_payloads.insert(format!("temperature:{}", port), payload);
                }
                SensorType::Light => {
                    let intensity = (146.0 - sample.raw_value as f64 / 7.0).floor().max(0.0).min(255.0) as u8;
                    let payload = ControlLabSensorPayload::Light(LightSensorPayload {
                        input_port: port,
                        raw_value: sample.raw_value,
                        intensity,
                    });
                    self.last_sensor_payloads.insert(format!("light:{}", port), payload);
                }
                SensorType::Rotation => {
                    let rotations = self.rotation_values[idx];
                    let payload = ControlLabSensorPayload::Rotation(RotationSensorPayload {
                        input_port: port,
                        raw_value: sample.raw_value,
                        rotations,
                        delta: sample.rotation_delta,
                    });
                    self.last_sensor_payloads.insert(format!("rotation:{}", port), payload);
                }
                SensorType::Unknown => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let cl = ControlLab::new("/dev/ttyUSB0");
        assert_eq!(cl.state(), ControlLabState::NotReady);
        assert!(!cl.is_connected());
    }

    #[test]
    fn test_set_sensor_type() {
        let mut cl = ControlLab::new("/dev/ttyUSB0");
        cl.set_sensor_type(1, SensorType::Touch);
        assert_eq!(cl.sensor_types[0], SensorType::Touch);
    }

    #[test]
    fn test_reset_rotation() {
        let mut cl = ControlLab::new("/dev/ttyUSB0");
        cl.rotation_values[0] = 42;
        cl.reset_rotation(1);
        assert_eq!(cl.rotation_values[0], 0);
    }

    #[test]
    fn test_read_no_data() {
        let cl = ControlLab::new("/dev/ttyUSB0");
        assert!(cl.read(1).is_none());
    }

    #[test]
    fn test_read_after_set_type_no_data() {
        let mut cl = ControlLab::new("/dev/ttyUSB0");
        cl.set_sensor_type(1, SensorType::Light);
        // No sensor data received yet
        assert!(cl.read(1).is_none());
    }

    #[test]
    fn test_handle_touch_notification() {
        let mut cl = ControlLab::new("/dev/ttyUSB0");
        cl.set_sensor_type(1, SensorType::Touch);

        let notification = SensorNotification {
            samples: vec![SensorSample {
                input_port: 1,
                raw_value: 500,
                state: 0,
                rotation_delta: 0,
            }],
        };
        cl.handle_sensor_notification(&notification);

        let payload = cl.read(1);
        assert!(payload.is_some());
        if let Some(ControlLabSensorPayload::Touch(t)) = payload {
            assert!(t.pressed);
            assert!(t.force > 0);
        } else {
            panic!("Expected touch payload");
        }
    }

    #[test]
    fn test_handle_rotation_notification() {
        let mut cl = ControlLab::new("/dev/ttyUSB0");
        cl.set_sensor_type(1, SensorType::Rotation);

        let notification = SensorNotification {
            samples: vec![SensorSample {
                input_port: 1,
                raw_value: 512,
                state: 5, // direction +, change = 1
                rotation_delta: 1,
            }],
        };
        cl.handle_sensor_notification(&notification);

        let payload = cl.read(1);
        assert!(payload.is_some());
        if let Some(ControlLabSensorPayload::Rotation(r)) = payload {
            assert_eq!(r.rotations, 1);
            assert_eq!(r.delta, 1);
        } else {
            panic!("Expected rotation payload");
        }
    }

    #[test]
    fn test_rotation_accumulates() {
        let mut cl = ControlLab::new("/dev/ttyUSB0");
        cl.set_sensor_type(1, SensorType::Rotation);

        for _ in 0..5 {
            cl.handle_sensor_notification(&SensorNotification {
                samples: vec![SensorSample {
                    input_port: 1,
                    raw_value: 512,
                    state: 5,
                    rotation_delta: 1,
                }],
            });
        }

        if let Some(ControlLabSensorPayload::Rotation(r)) = cl.read(1) {
            assert_eq!(r.rotations, 5);
        } else {
            panic!("Expected rotation");
        }
    }
}
