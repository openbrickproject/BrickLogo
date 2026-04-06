pub const HANDSHAKE_OUTBOUND: &[u8] = b"p\0###Do you byte, when I knock?$$$";
pub const HANDSHAKE_INBOUND: &[u8] = b"###Just a bit off the block!$$$";

pub const SENSOR_MESSAGE_LENGTH: usize = 19;
pub const SENSOR_MESSAGE_OFFSETS: [usize; 8] = [14, 10, 6, 2, 16, 12, 8, 4];

pub const OUTPUT_PORTS: [&str; 8] = ["A", "B", "C", "D", "E", "F", "G", "H"];

pub const DEFAULT_BAUD_RATE: u32 = 9600;
pub const KEEP_ALIVE_INTERVAL_MS: u64 = 2000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControlLabState {
    NotReady = 0,
    Ready = 1,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TouchEvent {
    Pressed = 1,
    Released = 0,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControlLabCommand {
    PowerOff = 0x90,
    PowerOn = 0x91,
    DirectionLeft = 0x93,
    DirectionRight = 0x94,
    PowerLevel = 0xb0,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SensorType {
    Unknown = 0,
    Touch = 1,
    Temperature = 2,
    Light = 3,
    Rotation = 4,
}

pub const INPUT_PORT_COUNT: usize = 8;
