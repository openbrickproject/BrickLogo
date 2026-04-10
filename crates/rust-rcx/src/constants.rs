// ── Serial / IR settings ─────────────────────────
pub const DEFAULT_BAUD_RATE: u32 = 2400;

// ── USB tower identifiers ────────────────────────
pub const USB_VENDOR_ID: u16 = 0x0694;
pub const USB_PRODUCT_ID: u16 = 0x0001;
pub const USB_TIMEOUT_MS: u64 = 1000;
pub const COMMAND_TIMEOUT_MS: u64 = 500;
pub const FIRMWARE_TIMEOUT_MS: u64 = 5000;
pub const COMMAND_RETRIES: usize = 3;

// ── Protocol framing ─────────────────────────────
pub const HEADER: [u8; 3] = [0x55, 0xFF, 0x00];

// ── Motor bitmask ────────────────────────────────
pub const MOTOR_A: u8 = 0x01;
pub const MOTOR_B: u8 = 0x02;
pub const MOTOR_C: u8 = 0x04;

// ── Motor direction bits ─────────────────────────
pub const DIR_FORWARD: u8 = 0x80;
pub const DIR_REVERSE: u8 = 0x00;
pub const DIR_FLIP: u8 = 0x40;

// ── Motor on/off bits ────────────────────────────
pub const MOTOR_ON: u8 = 0x80;
pub const MOTOR_OFF: u8 = 0x40;
pub const MOTOR_FLOAT: u8 = 0x00;

// ── Opcodes ──────────────────────────────────────
pub const OP_ALIVE: u8 = 0x10;
pub const OP_GET_VALUE: u8 = 0x12;
pub const OP_SET_MOTOR_POWER: u8 = 0x13;
pub const OP_SET_MOTOR_DIRECTION: u8 = 0xE1;
pub const OP_SET_MOTOR_ON_OFF: u8 = 0x21;
pub const OP_SET_SENSOR_TYPE: u8 = 0x32;
pub const OP_SET_SENSOR_MODE: u8 = 0x42;
pub const OP_PLAY_SOUND: u8 = 0x51;
pub const OP_PLAY_TONE: u8 = 0x23;
pub const OP_GET_BATTERY: u8 = 0x30;
pub const OP_CLEAR_SENSOR: u8 = 0xD1;
pub const OP_DELETE_FIRMWARE: u8 = 0x65;
pub const OP_START_FIRMWARE_DOWNLOAD: u8 = 0x75;
pub const OP_TRANSFER_DATA: u8 = 0x45;
pub const OP_UNLOCK_FIRMWARE: u8 = 0xA5;

// ── Firmware upload ──────────────────────────────
pub const FIRMWARE_BLOCK_SIZE: usize = 200;
pub const FIRMWARE_MAX_RETRIES: usize = 10;
pub const FIRMWARE_DELETE_KEY: [u8; 5] = [1, 3, 5, 7, 11];
pub const FIRMWARE_UNLOCK_KEY: [u8; 5] = [76, 69, 71, 79, 174]; // "LEGO" + 0xAE

// ── Sensor types ─────────────────────────────────
pub const SENSOR_TYPE_RAW: u8 = 0;
pub const SENSOR_TYPE_TOUCH: u8 = 1;
pub const SENSOR_TYPE_TEMPERATURE: u8 = 2;
pub const SENSOR_TYPE_LIGHT: u8 = 3;
pub const SENSOR_TYPE_ROTATION: u8 = 4;

// ── Sensor modes ─────────────────────────────────
pub const SENSOR_MODE_RAW: u8 = 0x00;
pub const SENSOR_MODE_BOOLEAN: u8 = 0x20;
pub const SENSOR_MODE_EDGE: u8 = 0x40;
pub const SENSOR_MODE_PULSE: u8 = 0x60;
pub const SENSOR_MODE_PERCENT: u8 = 0x80;
pub const SENSOR_MODE_CELSIUS: u8 = 0xA0;
pub const SENSOR_MODE_FAHRENHEIT: u8 = 0xC0;
pub const SENSOR_MODE_ANGLE: u8 = 0xE0;

// ── Source types (for get_value) ─────────────────
pub const SOURCE_SENSOR_VALUE: u8 = 9;
pub const SOURCE_RAW_SENSOR: u8 = 12;
pub const SOURCE_SENSOR_BOOLEAN: u8 = 13;

// ── Output port names ────────────────────────────
pub const OUTPUT_PORTS: [&str; 3] = ["A", "B", "C"];
pub const INPUT_PORTS: [&str; 3] = ["1", "2", "3"];
pub const INPUT_PORT_COUNT: usize = 3;

/// Map a port letter to its motor bitmask.
pub fn motor_mask(port: &str) -> Option<u8> {
    match port.to_uppercase().as_str() {
        "A" => Some(MOTOR_A),
        "B" => Some(MOTOR_B),
        "C" => Some(MOTOR_C),
        _ => None,
    }
}

/// Map an input port number string to sensor index (0-2).
pub fn sensor_index(port: &str) -> Option<u8> {
    match port {
        "1" => Some(0),
        "2" => Some(1),
        "3" => Some(2),
        _ => None,
    }
}

#[cfg(test)]
#[path = "tests/constants.rs"]
mod tests;
