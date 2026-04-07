use crate::constants::*;
use crate::protocol::*;
use serialport::SerialPort;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

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

/// Open a Control Lab serial port and perform the handshake.
/// Returns the port on success, ready for use.
pub fn connect(path: &str, baud_rate: u32) -> Result<Box<dyn SerialPort>, String> {
    let mut port = serialport::new(path, baud_rate)
        .timeout(Duration::from_millis(100))
        .open()
        .map_err(|e| format!("Failed to open serial port {}: {}", path, e))?;

    port.write_all(HANDSHAKE_OUTBOUND)
        .map_err(|e| format!("Handshake write failed: {}", e))?;
    port.flush()
        .map_err(|e| format!("Handshake flush failed: {}", e))?;

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut buffer = Vec::new();
    let mut read_buf = [0u8; 256];
    while Instant::now() < deadline {
        match port.read(&mut read_buf) {
            Ok(n) if n > 0 => buffer.extend_from_slice(&read_buf[..n]),
            _ => {}
        }
        if buffer
            .windows(HANDSHAKE_INBOUND.len())
            .any(|w| w == HANDSHAKE_INBOUND)
        {
            return Ok(port);
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    Err("Control Lab handshake timed out".to_string())
}

/// Process incoming sensor data from a read buffer and update sensor state.
pub fn process_sensor_data(
    read_buffer: &mut Vec<u8>,
    sensor_types: &[SensorType; INPUT_PORT_COUNT],
    rotation_values: &mut [i32; INPUT_PORT_COUNT],
    last_payloads: &mut HashMap<String, ControlLabSensorPayload>,
) {
    while read_buffer.len() >= SENSOR_MESSAGE_LENGTH {
        if read_buffer[0] != 0x00 {
            read_buffer.remove(0);
            continue;
        }

        let message: Vec<u8> = read_buffer[..SENSOR_MESSAGE_LENGTH].to_vec();
        if let Some(notification) = decode_sensor_message(&message) {
            *read_buffer = read_buffer[SENSOR_MESSAGE_LENGTH..].to_vec();

            for sample in &notification.samples {
                let port = sample.input_port;
                if port < 1 || port > INPUT_PORT_COUNT {
                    continue;
                }
                let idx = port - 1;

                rotation_values[idx] += sample.rotation_delta as i32;
                let sensor_type = sensor_types[idx];

                match sensor_type {
                    SensorType::Touch => {
                        let pressed_value = if sample.raw_value < 1000 {
                            TouchEvent::Pressed
                        } else {
                            TouchEvent::Released
                        };
                        let force = (100.0 - (sample.raw_value as f64 / 1024.0) * 100.0)
                            .max(0.0)
                            .min(100.0) as u8;
                        last_payloads.insert(
                            format!("touch:{}", port),
                            ControlLabSensorPayload::Touch(TouchSensorPayload {
                                input_port: port,
                                raw_value: sample.raw_value,
                                event: pressed_value,
                                pressed: pressed_value == TouchEvent::Pressed,
                                force,
                            }),
                        );
                    }
                    SensorType::Temperature => {
                        let fahrenheit = ((760.0 - sample.raw_value as f64) / 4.4 + 32.0 * 100.0)
                            .round()
                            / 100.0;
                        let celsius =
                            (((760.0 - sample.raw_value as f64) / 4.4) * (5.0 / 9.0) * 100.0)
                                .round()
                                / 100.0;
                        last_payloads.insert(
                            format!("temperature:{}", port),
                            ControlLabSensorPayload::Temperature(TemperatureSensorPayload {
                                input_port: port,
                                raw_value: sample.raw_value,
                                fahrenheit,
                                celsius,
                            }),
                        );
                    }
                    SensorType::Light => {
                        let intensity = (146.0 - sample.raw_value as f64 / 7.0)
                            .floor()
                            .max(0.0)
                            .min(255.0) as u8;
                        last_payloads.insert(
                            format!("light:{}", port),
                            ControlLabSensorPayload::Light(LightSensorPayload {
                                input_port: port,
                                raw_value: sample.raw_value,
                                intensity,
                            }),
                        );
                    }
                    SensorType::Rotation => {
                        let rotations = rotation_values[idx];
                        last_payloads.insert(
                            format!("rotation:{}", port),
                            ControlLabSensorPayload::Rotation(RotationSensorPayload {
                                input_port: port,
                                raw_value: sample.raw_value,
                                rotations,
                                delta: sample.rotation_delta,
                            }),
                        );
                    }
                    SensorType::Unknown => {}
                }
            }
        } else {
            read_buffer.clear();
            return;
        }
    }
}

#[cfg(test)]
#[path = "tests/controllab.rs"]
mod tests;
